import { useCallback, useRef, useEffect } from "react";

interface Props {
  onResize: (delta: number) => void;
}

export default function ResizableHandle({ onResize }: Props) {
  const dragging = useRef(false);
  const startX = useRef(0);

  const onMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    dragging.current = true;
    startX.current = e.clientX;
  }, []);

  useEffect(() => {
    const onMove = (e: MouseEvent) => {
      if (!dragging.current) return;
      const delta = startX.current - e.clientX;
      startX.current = e.clientX;
      onResize(delta);
    };
    const onUp = () => { dragging.current = false; };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
  }, [onResize]);

  return (
    <div
      className="relative w-1 flex-shrink-0 cursor-col-resize transition-colors hover:bg-ai/30 active:bg-ai/50"
      onMouseDown={onMouseDown}
    >
      <div className="absolute -inset-x-1 inset-y-0" />
    </div>
  );
}
