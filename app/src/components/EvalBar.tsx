interface EvalBarProps {
  winPct: number;
  orientation?: "white" | "black";
  label?: string;
}

export default function EvalBar({ winPct, orientation = "white", label }: EvalBarProps) {
  const clamped = Math.max(0, Math.min(100, winPct));
  const whiteAtBottom = orientation === "white";
  const whiteAdv = clamped >= 50;
  // Rótulo ancorado no extremo do lado vencedor, cor invertida p/ contraste.
  const labelAnchor = whiteAdv
    ? whiteAtBottom
      ? { bottom: 2 }
      : { top: 2 }
    : whiteAtBottom
      ? { top: 2 }
      : { bottom: 2 };
  const labelColor = whiteAdv ? "#262421" : "#f0f0f0";
  return (
    <div
      className="relative h-full w-5 shrink-0 overflow-hidden rounded-md border border-edge"
      style={{ backgroundColor: "#262421" }}
    >
      <div
        className="absolute left-0 right-0 bg-white transition-[height,top,bottom] duration-700 ease-[cubic-bezier(0.25,0.1,0.25,1)]"
        style={
          whiteAtBottom
            ? { bottom: 0, height: `${clamped}%` }
            : { top: 0, height: `${clamped}%` }
        }
      />
      <div className="absolute left-0 right-0 top-1/2 h-px -translate-y-1/2 bg-edge/60" />
      {label ? (
        <span
          className="pointer-events-none absolute left-1/2 -translate-x-1/2 text-[9px] font-semibold leading-none whitespace-nowrap"
          style={{ ...labelAnchor, color: labelColor }}
        >
          {label}
        </span>
      ) : null}
    </div>
  );
}
