import type { Classification } from '../types'

export interface VariationMove {
  id: string
  uci: string
  san: string
  fen: string
  classification?: Classification
  children: VariationMove[]
}

export function updateNodeClassification(
  nodes: VariationMove[],
  nodeId: string,
  value: Classification,
): VariationMove[] {
  let changed = false
  const next = nodes.map((node) => {
    if (node.id === nodeId) {
      changed = true
      return { ...node, classification: value }
    }
    const children = updateNodeClassification(node.children, nodeId, value)
    if (children === node.children) return node
    changed = true
    return { ...node, children }
  })
  return changed ? next : nodes
}

export function nodeAtPath(
  roots: VariationMove[],
  path: string[],
): VariationMove | null {
  let candidates = roots
  let current: VariationMove | null = null
  for (const id of path) {
    current = candidates.find((node) => node.id === id) ?? null
    if (!current) return null
    candidates = current.children
  }
  return current
}

export function appendAtPath(
  roots: VariationMove[],
  path: string[],
  move: VariationMove,
): VariationMove[] {
  if (path.length === 0) return [...roots, move]
  const [id, ...rest] = path
  return roots.map((node) =>
    node.id === id
      ? { ...node, children: appendAtPath(node.children, rest, move) }
      : node,
  )
}

export function childrenAtPath(
  roots: VariationMove[],
  path: string[],
): VariationMove[] {
  return path.length === 0 ? roots : (nodeAtPath(roots, path)?.children ?? [])
}

/** Insere uma linha sem duplicar prefixos nem alterar snapshots anteriores. */
export function mergeLineAtPath(
  roots: VariationMove[],
  path: string[],
  line: VariationMove,
): VariationMove[] {
  if (path.length > 0) {
    const [id, ...rest] = path
    return roots.map((node) => {
      if (node.id !== id) return node
      const children = mergeLineAtPath(node.children, rest, line)
      return children === node.children ? node : { ...node, children }
    })
  }
  const existing = roots.find((node) => node.uci === line.uci)
  if (!existing) return [...roots, line]
  const continuation = line.children[0]
  if (!continuation) return roots
  const children = mergeLineAtPath(existing.children, [], continuation)
  if (children === existing.children) return roots
  return roots.map((node) => (node === existing ? { ...node, children } : node))
}
