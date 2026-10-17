export function inspectPixels(context: WebGL2RenderingContext | WebGLRenderingContext, width: number, height: number) {
  const pixels = new Uint8Array(width * height * 4);
  context.readPixels(0, 0, width, height, context.RGBA, context.UNSIGNED_BYTE, pixels);
  let visible = 0;
  let transparent = 0;
  let partial = 0;
  let edge = 0;
  for (let i = 0; i < width * height; i++) {
    const alpha = pixels[i * 4 + 3];
    if (alpha === 0) transparent++;
    else {
      visible++;
      if (alpha < 255) partial++;
      const x = i % width;
      const y = Math.floor(i / width);
      if (x < 2 || x >= width - 2 || y < 2 || y >= height - 2) edge++;
    }
  }
  return { visible, transparent, partial, edge };
}
