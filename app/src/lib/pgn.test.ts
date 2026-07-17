import { describe, it, expect } from "vitest";
import { parsePgn, resultLabel } from "./pgn";

const FULL_PGN = [
  '[Event "Torneio Teste"]',
  '[Site "EngineRoom"]',
  '[Date "2026.01.01"]',
  '[White "Alice"]',
  '[Black "Bob"]',
  '[WhiteElo "1850"]',
  "[BlackElo \"1800\"]",
  '[Result "1-0"]',
  "",
  "1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O Be7 1-0",
].join("\n");

describe("parsePgn", () => {
  it("parses headers and move count from a full PGN", () => {
    const r = parsePgn(FULL_PGN);
    expect(r.ok).toBe(true);
    if (!r.ok) return;
    expect(r.meta).toMatchObject({
      white: "Alice",
      black: "Bob",
      whiteElo: "1850",
      blackElo: "1800",
      result: "1-0",
      event: "Torneio Teste",
    });
    // 10 plies -> 5 full moves
    expect(r.meta.plies).toBe(10);
  });

  it("parses movetext-only PGN with default players", () => {
    const r = parsePgn("1. e4 e5 2. Nf3 Nc6 1/2-1/2");
    expect(r.ok).toBe(true);
    if (!r.ok) return;
    expect(r.meta.white).toBe("Jogador");
    expect(r.meta.black).toBe("Jogador");
    expect(r.meta.result).toBe("1/2-1/2");
    expect(r.meta.plies).toBe(4);
  });

  it("rejects empty input", () => {
    expect(parsePgn("")).toEqual({
      ok: false,
      error: "Cole um PGN ou selecione um arquivo.",
    });
    expect(parsePgn("   \n  ").ok).toBe(false);
  });

  it("rejects garbage text", () => {
    const r = parsePgn("isso definitivamente nao e um pgn");
    expect(r.ok).toBe(false);
  });

  it("rejects headers with no moves", () => {
    const r = parsePgn('[White "A"]\n[Black "B"]\n\n');
    expect(r.ok).toBe(false);
  });
});

describe("resultLabel", () => {
  it("maps standard results", () => {
    expect(resultLabel("1-0")).toBe("1–0 Brancas");
    expect(resultLabel("0-1")).toBe("0–1 Pretas");
    expect(resultLabel("1/2-1/2")).toBe("Empate");
    expect(resultLabel("*")).toBe("Em andamento");
  });
});
