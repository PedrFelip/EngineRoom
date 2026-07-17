import { describe, it, expect } from "vitest";
import { lookupEco, type EcoEntry } from "./eco";

const DS: EcoEntry[] = [
  { code: "B00", name: "King's Pawn", moves: ["e4"] },
  { code: "C20", name: "Open Game", moves: ["e4", "e5"] },
  { code: "C50", name: "Italian Game", moves: ["e4", "e5", "Nf3", "Nc6", "Bc4"] },
];

describe("lookupEco", () => {
  it("retorna null quando não há casamento", () => {
    expect(lookupEco(["d4", "d5"], DS)).toBeNull();
    expect(lookupEco([], DS)).toBeNull();
  });

  it("casamento exato retorna a entrada", () => {
    expect(lookupEco(["e4", "e5"], DS)).toEqual(DS[1]);
  });

  it("casamento por prefixo (jogo mais longo que a linha)", () => {
    expect(lookupEco(["e4", "e5", "Nf3", "Nc6", "Bc4", "Bc5"], DS)).toEqual(DS[2]);
  });

  it("desempata pela linha mais longa (mais específica)", () => {
    expect(lookupEco(["e4", "e5", "Nf3", "Nc6", "Bc4"], DS)).toEqual(DS[2]);
    expect(lookupEco(["e4", "e5", "Nf3"], DS)).toEqual(DS[1]);
  });

  it("ignora linhas mais longas que o jogo jogado", () => {
    expect(lookupEco(["e4", "e5", "Nf3"], DS).code).toBe("C20");
  });
});
