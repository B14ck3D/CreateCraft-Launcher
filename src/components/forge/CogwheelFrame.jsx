export default function CogwheelFrame({ children, className = '' }) {
  return (
    <div className={`bg-card/80 backdrop-blur-sm pixel-border ${className}`}>{children}</div>
  );
}
