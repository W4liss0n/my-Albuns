import { Filter, GlProgram } from "pixi.js";

// Integer sRGB luminance from the accepted renderer contract (design 0019).
const weights = [54, 183, 19] as const;
const matrixRow = [...weights.map((weight) => weight / 256), 0, 0];
export const PHOTO_BLACK_AND_WHITE_SVG_MATRIX = [...matrixRow, ...matrixRow, ...matrixRow, 0, 0, 0, 1, 0].join(" ");

// Standard PixiJS v8 filter coordinates; the production Canvas uses WebGL.
const vertex = `
in vec2 aPosition;
out vec2 vTextureCoord;
uniform vec4 uInputSize;
uniform vec4 uOutputFrame;
uniform vec4 uOutputTexture;
void main() {
  vec2 position = aPosition * uOutputFrame.zw + uOutputFrame.xy;
  position.x = position.x * (2.0 / uOutputTexture.x) - 1.0;
  position.y = position.y * (2.0 * uOutputTexture.z / uOutputTexture.y) - uOutputTexture.z;
  gl_Position = vec4(position, 0.0, 1.0);
  vTextureCoord = aPosition * (uOutputFrame.zw * uInputSize.zw);
}`;

const fragment = `
in vec2 vTextureCoord;
out vec4 finalColor;
uniform sampler2D uTexture;
void main() {
  vec4 color = texture(uTexture, vTextureCoord);
  if (color.a == 0.0) {
    finalColor = vec4(0.0);
    return;
  }
  vec3 rgb = floor(color.rgb / color.a * 255.0 + 0.5);
  float luminance = floor((dot(rgb, vec3(${weights.map((weight) => `${weight}.0`).join(", ")})) + 128.0) / 256.0) / 255.0;
  finalColor = vec4(vec3(luminance) * color.a, color.a);
}`;

export function createPhotoBlackAndWhiteFilter() {
  // Own the program so destroying a Photo never invalidates another Photo's filter.
  return new Filter({
    glProgram: new GlProgram({ name: "photo-black-and-white", vertex, fragment, preferredFragmentPrecision: "highp" }),
    resolution: "inherit",
    antialias: "inherit",
  });
}
