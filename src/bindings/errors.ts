export class EngineBindingError extends Error {
  constructor(message: string) { super(message); this.name = 'EngineBindingError'; }
}
export function bindingError(error: unknown): EngineBindingError {
  return new EngineBindingError(error instanceof Error ? error.message : String(error));
}
