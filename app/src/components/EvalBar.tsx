interface EvalBarProps {
  winPct: number;
  orientation?: "white" | "black";
}

export default function EvalBar({ winPct, orientation = "white" }: EvalBarProps) {
  const clamped = Math.max(0, Math.min(100, winPct));
  const whiteAtBottom = orientation === "white";
  return (
    <div
      className="relative h-full w-5 shrink-0 overflow-hidden rounded-md border border-edge"
      style={{ backgroundColor: "#262421" }}
    >
      <div
        className="absolute left-0 right-0 bg-white transition-[height,top,bottom] duration-300 ease-out"
        style={
          whiteAtBottom
            ? { bottom: 0, height: `${clamped}%` }
            : { top: 0, height: `${clamped}%` }
        }
      />
      <div className="absolute left-0 right-0 top-1/2 h-px -translate-y-1/2 bg-edge/60" />
    </div>
  );
}
