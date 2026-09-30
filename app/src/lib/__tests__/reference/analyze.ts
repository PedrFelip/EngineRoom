/** API pública do pipeline de análise. */

export * from './analysis/analysis-types'
export { analyzeGameAdaptive } from './analysis/analyze-adaptive'
export { analyzeGame } from './analysis/analyze-standard'
export { configureEngine } from './analysis/engine-analysis'
export { accuracyByPhaseOf, buildReview } from './analysis/review-builder'
