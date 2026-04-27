const SIZE_PATTERN = /^\s*(\d+)\s*[xX×]\s*(\d+)\s*$/;
const RATIO_PATTERN = /^\s*(\d+(?:\.\d+)?)\s*[:xX×]\s*(\d+(?:\.\d+)?)\s*$/;

export type SizeTier = '1K' | '2K' | '4K';

const roundToMultiple = (value: number, multiple: number): number => {
  return Math.max(multiple, Math.round(value / multiple) * multiple);
};

export const normalizeImageSize = (size: string): string => {
  const trimmed = size.trim();
  const match = trimmed.match(SIZE_PATTERN);
  if (!match) return trimmed;

  const width = roundToMultiple(Number(match[1]), 16);
  const height = roundToMultiple(Number(match[2]), 16);
  return `${width}x${height}`;
};

export const parseRatio = (ratio: string): { width: number; height: number } | null => {
  const match = ratio.match(RATIO_PATTERN);
  if (!match) return null;

  const width = Number(match[1]);
  const height = Number(match[2]);
  if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) {
    return null;
  }

  return { width, height };
};

export const calculateImageSize = (tier: SizeTier, ratio: string): string | null => {
  const parsed = parseRatio(ratio);
  if (!parsed) return null;

  const { width: ratioWidth, height: ratioHeight } = parsed;
  if (ratioWidth === ratioHeight) {
    const side = tier === '1K' ? 1024 : tier === '2K' ? 2048 : 3840;
    return `${side}x${side}`;
  }

  if (tier === '1K') {
    const shortSide = 1024;
    const width = ratioWidth > ratioHeight
      ? roundToMultiple(shortSide * ratioWidth / ratioHeight, 16)
      : shortSide;
    const height = ratioWidth > ratioHeight
      ? shortSide
      : roundToMultiple(shortSide * ratioHeight / ratioWidth, 16);
    return `${width}x${height}`;
  }

  const longSide = tier === '2K' ? 2048 : 3840;
  const width = ratioWidth > ratioHeight
    ? longSide
    : roundToMultiple(longSide * ratioWidth / ratioHeight, 16);
  const height = ratioWidth > ratioHeight
    ? roundToMultiple(longSide * ratioHeight / ratioWidth, 16)
    : longSide;
  return `${width}x${height}`;
};
