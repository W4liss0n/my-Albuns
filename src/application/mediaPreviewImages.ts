// Mounted thumbnails lend their loaded image to the Canvas. This registry owns
// neither a second decode nor GPU resources; unmounting releases its reference.
const imagesByUrl = new Map<string, Set<HTMLImageElement>>();

export function registerMediaPreviewImage(url: string, image: HTMLImageElement) {
  const images = imagesByUrl.get(url) ?? new Set<HTMLImageElement>();
  images.add(image);
  imagesByUrl.set(url, images);
  return () => {
    images.delete(image);
    if (images.size === 0) imagesByUrl.delete(url);
  };
}

export function loadedMediaPreviewImage(url: string) {
  for (const image of imagesByUrl.get(url) ?? []) {
    if (image.complete && image.naturalWidth > 0 && image.naturalHeight > 0 &&
      image.crossOrigin === "anonymous" && image.getAttribute("src") === url &&
      image.currentSrc === image.src) return image;
  }
  return undefined;
}
