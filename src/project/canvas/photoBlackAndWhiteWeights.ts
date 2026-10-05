// Integer sRGB luminance from the accepted renderer contract (design 0019).
export const PHOTO_BLACK_AND_WHITE_WEIGHTS = [54, 183, 19] as const;
const matrixRow = [...PHOTO_BLACK_AND_WHITE_WEIGHTS.map((weight) => weight / 256), 0, 0];
export const PHOTO_BLACK_AND_WHITE_SVG_MATRIX = [...matrixRow, ...matrixRow, ...matrixRow, 0, 0, 0, 1, 0].join(" ");
