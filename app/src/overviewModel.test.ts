import { describe, expect, it } from 'vitest';
import {
  OVERVIEW_HISTORY_LIMIT,
  clampPercent,
  drivePercent,
  formatWholePercent,
  memoryPercent,
  pushHistorySample,
  sparklineGeometry,
} from './overviewModel';

describe('overviewModel percentages', () => {
  it('clamps invalid and out-of-range percentages safely', () => {
    expect(clampPercent(Number.NaN)).toBe(0);
    expect(clampPercent(Number.POSITIVE_INFINITY)).toBe(0);
    expect(clampPercent(-12)).toBe(0);
    expect(clampPercent(142)).toBe(100);
    expect(clampPercent(37.4)).toBe(37.4);
    expect(formatWholePercent(37.6)).toBe('38%');
  });

  it('computes memory and drive percentages safely', () => {
    expect(memoryPercent(0, 0)).toBe(0);
    expect(memoryPercent(8, 32)).toBe(25);
    expect(memoryPercent(40, 32)).toBe(100);

    expect(drivePercent(0, 0)).toBe(0);
    expect(drivePercent(25, 100)).toBe(75);
    expect(drivePercent(120, 100)).toBe(0);
    expect(drivePercent(-10, 100)).toBe(100);
  });
});

describe('pushHistorySample', () => {
  it('caps history to OVERVIEW_HISTORY_LIMIT and keeps the newest samples', () => {
    let history: number[] = [];
    for (let i = 1; i <= OVERVIEW_HISTORY_LIMIT + 5; i += 1) {
      history = pushHistorySample(history, i);
    }
    expect(history).toHaveLength(OVERVIEW_HISTORY_LIMIT);
    expect(history[0]).toBe(6);
    expect(history[history.length - 1]).toBe(OVERVIEW_HISTORY_LIMIT + 5);
  });
});

describe('sparklineGeometry', () => {
  it('handles empty and single-sample histories without NaN', () => {
    expect(sparklineGeometry([], 300, 100)).toEqual({
      linePoints: '',
      areaPath: '',
      lastPoint: null,
    });

    const single = sparklineGeometry([50], 290, 100, 30, 10);
    expect(single.lastPoint).toEqual({ x: 290, y: 50 });
    expect(single.linePoints).toContain('290,50');
    expect(single.areaPath).toMatch(/^M .* Z$/);
  });

  it('right-aligns partial histories and spans full width when full', () => {
    const partial = sparklineGeometry([0, 100], 290, 100, 30, 10);
    expect(partial.linePoints).toBe('280,90 290,10');
    expect(partial.lastPoint).toEqual({ x: 290, y: 10 });

    const full = sparklineGeometry(Array.from({ length: 30 }, () => 50), 290, 100, 30, 10);
    expect(full.linePoints.startsWith('0,50 ')).toBe(true);
    expect(full.lastPoint).toEqual({ x: 290, y: 50 });

    const inset = sparklineGeometry([25, 75], 300, 100, 30, 10, 10);
    expect(inset.lastPoint).toEqual({ x: 290, y: 30 });
  });
});
