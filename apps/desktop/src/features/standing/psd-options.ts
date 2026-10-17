import type { PsdProbe } from '@sungeki/api-client';

const parent = (path: string) => path.slice(0, path.lastIndexOf('/'));
export function psdOptions(probe: PsdProbe, minimumOptions = 2) {
  const names = new Map(probe.layers.map(layer => [layer.path, layer.name.replace(/^[!*]+/, '')]));
  const children = new Map<string, typeof probe.layers>();
  for (const layer of probe.layers) {
    if (!layer.name.startsWith('*')) continue;
    const group = parent(layer.path);
    const list = children.get(group) ?? [];
    list.push(layer); children.set(group, list);
  }
  return probe.layers.filter(layer => layer.kind === 'group' && (children.get(layer.path)?.length ?? 0) >= minimumOptions)
    .map(layer => {
      const hierarchy: string[] = [];
      for (let path = layer.path; path; path = parent(path)) hierarchy.unshift(names.get(path) ?? '名称なし');
      return { path: layer.path, label: hierarchy.join(' / '),
        options: children.get(layer.path)!.map(child => ({ path: child.path, label: names.get(child.path) || '名称なし' })) };
    });
}
