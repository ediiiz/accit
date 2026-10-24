import { describe, expect, test } from "bun:test";
import { percentile } from "./stats";

const upTo = (n: number) => Array.from({ length: n }, (_, index) => index + 1);

describe("percentile (nearest rank)", () => {
  test("p95 of 1..100 is 95", () => {
    expect(percentile(upTo(100), 0.95)).toBe(95);
  });

  test("p95 of 1..20 is 19", () => {
    expect(percentile(upTo(20), 0.95)).toBe(19);
  });

  test("does not depend on input order", () => {
    expect(percentile([5, 1, 4, 2, 3], 0.5)).toBe(3);
  });

  test("one sample is every percentile", () => {
    expect(percentile([7], 0.95)).toBe(7);
  });

  test("p100 is the maximum", () => {
    expect(percentile([3, 9, 1], 1)).toBe(9);
  });

  test("no samples is an error, not a passing number", () => {
    expect(() => percentile([], 0.95)).toThrow();
  });
});
