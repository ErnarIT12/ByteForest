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
  it('handles empty and single-sample histories cleanly full-width without NaN', () => {
    expect(sparklineGeometry([], 300, 100)).toEqual({
      linePoints: '',
      smoothLinePath: '',
      areaPath: '',
      lastPoint: null,
    });

    const single = sparklineGeometry([50], 300, 100, 30, 10);
    expect(single.lastPoint).toEqual({ x: 300, y: 50 });
    expect(single.smoothLinePath).toBe('M 0 50 L 300 50');
    expect(single.areaPath).toBe('M 0 100 L 0 50 L 300 50 L 300 100 Z');
  });

  it('spans full width with monotonic cubic Bezier curves and respects padRight', () => {
    const pair = sparklineGeometry([0, 100], 300, 100, 30, 10);
    expect(pair.linePoints).toBe('0,90 300,10');
    expect(pair.smoothLinePath).toBe('M 0 90 C 150 90, 150 10, 300 10');
    expect(pair.areaPath).toBe('M 0 100 L 0 90 C 150 90, 150 10, 300 10 L 300 100 Z');
    expect(pair.lastPoint).toEqual({ x: 300, y: 10 });

    const inset = sparklineGeometry([25, 75], 300, 100, 30, 10, 10);
    expect(inset.lastPoint).toEqual({ x: 290, y: 30 });
  });
});
