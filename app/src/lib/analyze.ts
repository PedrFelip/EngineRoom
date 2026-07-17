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

export interface RawPosition {
  fen: string;
  cp: number;
  depth: number;
  pv: string[];
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
    return {
      ply: i,
      fen: r.fen,
      depth: r.depth,
      cp: r.cp,
      winPct,
      pv: r.pv,
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

async function evalPosition(
  port: EnginePort,
  fen: string,
  depth: number,
): Promise<RawPosition> {
  let best: { depth: number; score?: InfoScore; pv: string[] } = {
    depth: 0,
    pv: [],
  };
  await port.send(`position fen ${fen}`);
  await ask(port, `go depth ${depth}`, (line) => {
    const info = parseInfo(line);
    if (info?.score && (info.multipv ?? 1) === 1 && (info.depth ?? 0) >= best.depth) {
      best = { depth: info.depth ?? 0, score: info.score, pv: info.pv ?? [] };
    }
    return line.trim().startsWith("bestmove");
  });
  const cp = scoreToCp(best.score) ?? 0;
  return { fen, cp, depth: best.depth, pv: best.pv };
}

/**
 * Aciona o engine posição a posição e devolve a revisão completa.
 * `port` abstrai o processo UCI (sidecar Tauri ou engine falso em testes).
 */
export async function analyzeGame(
  pgn: string,
  depth: number,
  port: EnginePort,
): Promise<ReviewResult> {
  const { positionFens, moves } = extractGame(pgn);

  await ask(port, "uci", isUciOk);
  await ask(port, "isready", isReadyOk);

  const raw: RawPosition[] = [];
  for (let i = 0; i < positionFens.length; i++) {
    raw.push(await evalPosition(port, positionFens[i], depth));
  }
  await port.send("quit");

  const opening = await lookupOpening(moves.map((m) => m.san));
  const book: BookInfo | undefined = opening
    ? { maxPly: opening.moves.length, eco: opening }
    : undefined;

  return buildReview({ startFen: positionFens[0], moves }, raw, book);
}
