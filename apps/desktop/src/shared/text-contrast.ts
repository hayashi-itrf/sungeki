/** Choose black or white for contrast against the current solid canvas background. */
export function contrastingTextColor(background: string): string {
  if (!/^#[0-9a-f]{6}$/i.test(background)) return '#000000';
  const channels = [1, 3, 5].map(offset => {
    const channel = parseInt(background.slice(offset, offset + 2), 16) / 255;
    return channel <= .04045 ? channel / 12.92 : ((channel + .055) / 1.055) ** 2.4;
  });
  const luminance = channels[0]! * .2126 + channels[1]! * .7152 + channels[2]! * .0722;
  return luminance > .179 ? '#000000' : '#ffffff';
}
