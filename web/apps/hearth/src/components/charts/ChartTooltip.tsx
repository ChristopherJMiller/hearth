import type { TooltipProps } from 'recharts';

export const chartTooltipContent: TooltipProps<number, string>['contentStyle'] = {
  background: 'var(--color-surface-popover)',
  border: '1px solid var(--color-border)',
  borderRadius: 'var(--radius-sm)',
  fontSize: 12,
  color: 'var(--color-text-primary)',
  padding: '8px 12px',
};

export const chartAxisTick = {
  fontSize: 11,
  fill: 'var(--chart-axis)',
};

export const chartGridStroke = 'var(--chart-grid)';
