/**
 * Orquestração da revisão: converte uma partida jogada + avaliações brutas do
 * engine em um ReviewResult completo (win%, classificação de lances e precisão).
 *
 * `buildReview` é puro (sem engine, sem async) — toda a matemática de sinal,
 * perda de win% e classificação mora aqui. `analyzeGame` é a cola de I/O que
 * aciona o engine via um "port" injetável (testável com engine falso).
 */

import { Chess } from "chess.js";
import type {
  AccuracyByColor,
  MoveAnalysis,
  PositionAnalysis,
  PvLine,
  ReviewResult,
} from "../types";
import { classifyMove, cpToWinPct, gameAccuracy } from "./scoring";
import { isReadyOk, isUciOk, parseInfo, scoreToCp } from "./uci";
import type { InfoScore } from "./uci";
import { lookupOpening, type EcoEntry } from "./eco";

export interface BookInfo {
  maxPly: number;
  eco: EcoEntry | null;
}

export interface RawLine {
  multipv: number;
  cp: number;
  pv: string[];
  san?: string | null;
}

export interface RawPosition {
  fen: string;
  cp: number;
  depth: number;
  pv: string[];
  lines?: RawLine[];
}

export interface PlayedMove {
  ply: number;
  color: "w" | "b";
  san: string;
  uci: string;
  fenBefore: string;
}

export interface PlayedGame {
  startFen: string;
  moves: PlayedMove[];
}

const opposite = (c: "w" | "b"): "w" | "b" => (c === "w" ? "b" : "w");

function sideToMoveAt(game: PlayedGame, ply: number): "w" | "b" {
  if (ply < game.moves.length) return game.moves[ply].color;
  return opposite(game.moves[game.moves.length - 1].color);
}

/**
 * Constrói a revisão a partir da partida jogada e das avaliações brutas por ply.
 * `raw[i]` é a avaliação da posição após o i-ésimo ply (raw[0] = posição inicial).
 * O win% das posições é normalizado para o ponto de vista das brancas.
 */
export function buildReview(
  game: PlayedGame,
  raw: RawPosition[],
  book?: BookInfo,
): ReviewResult {
  const positions: PositionAnalysis[] = raw.map((r, i) => {
    const stm = sideToMoveAt(game, i);
    const winPct = stm === "w" ? cpToWinPct(r.cp) : 100 - cpToWinPct(r.cp);
    const rawLines = r.lines ?? [{ multipv: 1, cp: r.cp, pv: r.pv }];
    const lines: PvLine[] = rawLines.map((l) => ({
      multipv: l.multipv,
      san: l.san ?? null,
      cp: stm === "w" ? l.cp : -l.cp,
      winPct: stm === "w" ? cpToWinPct(l.cp) : 100 - cpToWinPct(l.cp),
      pv: l.pv,
    }));
    return {
      ply: i,
      fen: r.fen,
      depth: r.depth,
      cp: r.cp,
      winPct,
      pv: r.pv,
      lines,
    };
  });

  const moves: MoveAnalysis[] = game.moves.map((m) => {
    const before = raw[m.ply - 1];
    const after = raw[m.ply];
    const winPctBefore = cpToWinPct(before.cp);
    const winPctAfter = 100 - cpToWinPct(after.cp);
    const winPctLoss = Math.max(0, winPctBefore - winPctAfter);
    const isBook = !!book && m.ply <= book.maxPly;
    return {
      ply: m.ply,
      color: m.color,
      san: m.san,
      uci: m.uci,
      fenBefore: m.fenBefore,
      classification: classifyMove(winPctLoss, isBook),
      winPctBefore,
      winPctAfter,
      winPctLoss,
      bestUci: before.pv[0] ?? null,
      isBook,
      eco: isBook && book?.eco ? { code: book.eco.code, name: book.eco.name } : null,
    };
  });

  const accuracy: AccuracyByColor = {
    white: gameAccuracy(
      moves.filter((m) => m.color === "w" && !m.isBook).map((m) => m.winPctLoss),
    ),
    black: gameAccuracy(
      moves.filter((m) => m.color === "b" && !m.isBook).map((m) => m.winPctLoss),
    ),
  };

  return { positions, moves, accuracy };
}

/** Interface do engine injetável, para testar com engine falso. */
export interface EnginePort {
  send(cmd: string): void | Promise<void>;
  onLine(handler: (line: string) => void): () => void;
}

interface ExtractedGame {
  positionFens: string[];
  moves: PlayedMove[];
}

function extractGame(pgn: string): ExtractedGame {
  const chess = new Chess();
  chess.loadPgn(pgn);
  const verbose = chess.history({ verbose: true });
  const replay = new Chess();
  const positionFens: string[] = [replay.fen()];
  const moves: PlayedMove[] = [];
  verbose.forEach((m, i) => {
    const fenBefore = replay.fen();
    replay.move({ from: m.from, to: m.to, promotion: m.promotion });
    positionFens.push(replay.fen());
    moves.push({
      ply: i + 1,
      color: m.color,
      san: m.san,
      uci: m.from + m.to + (m.promotion ?? ""),
      fenBefore,
    });
  });
  return { positionFens, moves };
}

function ask(port: EnginePort, cmd: string, done: (line: string) => boolean): Promise<void> {
  return new Promise((resolve) => {
    const off = port.onLine((line) => {
      if (done(line)) {
        off();
        resolve();
      }
    });
    void port.send(cmd);
  });
}

function uciToSan(fen: string, uci: string): string | null {
  try {
    const c = new Chess(fen);
    const m = c.move({
      from: uci.slice(0, 2),
      to: uci.slice(2, 4),
      promotion: uci[4],
    });
    return m ? m.san : null;
  } catch {
    return null;
  }
}

async function evalPosition(
  port: EnginePort,
  fen: string,
  depth: number,
): Promise<RawPosition> {
  const byPv = new Map<number, { depth: number; score?: InfoScore; pv: string[] }>();
  await port.send(`position fen ${fen}`);
  await ask(port, `go depth ${depth}`, (line) => {
    const info = parseInfo(line);
    if (info?.score) {
      const idx = info.multipv ?? 1;
      const prev = byPv.get(idx);
      if (!prev || (info.depth ?? 0) >= prev.depth) {
        byPv.set(idx, { depth: info.depth ?? 0, score: info.score, pv: info.pv ?? [] });
      }
    }
    return line.trim().startsWith("bestmove");
  });
  const lines: RawLine[] = [...byPv.entries()]
    .sort((a, b) => a[0] - b[0])
    .map(([multipv, l]) => ({ multipv, cp: scoreToCp(l.score) ?? 0, pv: l.pv }));
  const principal = lines.find((l) => l.multipv === 1) ?? lines[0];
  return {
    fen,
    cp: principal?.cp ?? 0,
    depth: byPv.get(1)?.depth ?? 0,
    pv: principal?.pv ?? [],
    lines,
  };
}

function terminalCp(fen: string): number | null {
  try {
    const c = new Chess(fen);
    if (c.isCheckmate()) return -100000;
    if (c.isGameOver()) return 0;
    return null;
  } catch {
    return null;
  }
}

/**
 * Aciona o engine posição a posição e devolve a revisão completa.
 * `port` abstrai o processo UCI (sidecar Tauri ou engine falso em testes).
 * `multipv` define quantas linhas candidatas o engine retorna por posição.
 * `opts.threads` / `opts.hashMb` dimensionam a engine (Threads/Hash) para o uso
 * ideal de CPU/RAM. Omitir mantém os defaults do Stockfish.
 * Posições terminais (xeque-mate/afogamento) são resolvidas sem chamar a engine.
 */
export async function analyzeGame(
  pgn: string,
  depth: number,
  port: EnginePort,
  multipv = 1,
  opts: { threads?: number; hashMb?: number } = {},
): Promise<ReviewResult> {
  const { positionFens, moves } = extractGame(pgn);

  await ask(port, "uci", isUciOk);
  await ask(port, "isready", isReadyOk);
  if (opts.threads && opts.threads > 1) {
    await port.send(`setoption name Threads value ${opts.threads}`);
  }
  if (opts.hashMb && opts.hashMb > 0) {
    await port.send(`setoption name Hash value ${opts.hashMb}`);
  }
  await port.send(`setoption name Multipv value ${Math.max(1, multipv)}`);

  const raw: RawPosition[] = [];
  for (let i = 0; i < positionFens.length; i++) {
    const fen = positionFens[i];
    const term = terminalCp(fen);
    let pos: RawPosition;
    if (term !== null) {
      pos = {
        fen,
        cp: term,
        depth: 0,
        pv: [],
        lines: [{ multipv: 1, cp: term, pv: [] }],
      };
    } else {
      pos = await evalPosition(port, fen, depth);
      for (const l of pos.lines ?? []) {
        l.san = l.pv[0] ? uciToSan(pos.fen, l.pv[0]) : null;
      }
    }
    raw.push(pos);
  }
  await port.send("quit");

  const opening = await lookupOpening(moves.map((m) => m.san));
  const book: BookInfo | undefined = opening
    ? { maxPly: opening.moves.length, eco: opening }
    : undefined;

  return buildReview({ startFen: positionFens[0], moves }, raw, book);
}
