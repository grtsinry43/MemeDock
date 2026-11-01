/** A rolling tap window: complete the required taps inside the window to fire once. */
export class TapSequence {
  readonly #required: number;
  readonly #windowMs: number;
  #taps: number[] = [];

  constructor(required = 7, windowMs = 3_000) {
    this.#required = required;
    this.#windowMs = windowMs;
  }

  tap(now = performance.now()): boolean {
    const cutoff = now - this.#windowMs;
    this.#taps = this.#taps.filter(time => time >= cutoff);
    this.#taps.push(now);
    if (this.#taps.length < this.#required) return false;
    this.#taps = [];
    return true;
  }
}
