/** Per-frame overrides, shared by interactive and offline rendering. */
export type ExpressionWeights = Readonly<Record<string, number>>;

type Manager = {
  getValue(name: string): number | null;
  setValue(name: string, value: number): void;
  update(): void;
};

export function validateExpressions(manager: Manager | undefined, weights: ExpressionWeights) {
  const entries = Object.entries(weights);
  if (entries.length > 256) throw new Error('表情指定は256件までです');
  for (const [name, value] of entries) {
    if (!name || !Number.isFinite(value) || value < 0 || value > 1) {
      throw new Error(`表情ウェイトが不正です: ${name}`);
    }
    if (manager?.getValue(name) == null) throw new Error(`モデルにない表情です: ${name}`);
  }
}

/** Restore VRMA values even if rendering fails; overrides never alter simulation history. */
export function withExpressions<T>(manager: Manager | undefined, weights: ExpressionWeights, render: () => T): T {
  validateExpressions(manager, weights);
  if (!manager || Object.keys(weights).length === 0) return render();
  const previous = Object.keys(weights).map(name => [name, manager.getValue(name)!] as const);
  try {
    for (const [name, value] of Object.entries(weights)) manager.setValue(name, value);
    manager.update();
    return render();
  } finally {
    for (const [name, value] of previous) manager.setValue(name, value);
    manager.update();
  }
}
