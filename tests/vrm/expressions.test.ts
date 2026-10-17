import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withExpressions } from '../../packages/vrm/src/expressions.ts';

function fixture() {
  const values = new Map([['aa', .3], ['happy', .2]]);
  let updates = 0;
  const manager = {
    getValue: (name: string) => values.get(name) ?? null,
    setValue: (name: string, value: number) => { values.set(name, value); },
    update: () => { updates++; },
  };
  return { values, manager, updates: () => updates };
}

test('per-frame mouth/face overrides preserve animation values and never leak to repeated frames', () => {
  const f = fixture();
  assert.deepEqual(withExpressions(f.manager, { aa: 1, happy: 0 }, () => [...f.values.values()]), [1, 0]);
  assert.deepEqual([...f.values.values()], [.3, .2]);
  assert.deepEqual(withExpressions(f.manager, {}, () => [...f.values.values()]), [.3, .2]);
  assert.equal(f.updates(), 2);
  assert.throws(() => withExpressions(f.manager, { aa: 0 }, () => { throw Error('render'); }), /render/);
  assert.deepEqual([...f.values.values()], [.3, .2]);
});

test('unknown expressions and invalid weights reject the whole override before mutation', () => {
  for (const weights of [{ aa: 1, missing: 1 }, { aa: NaN }, { aa: Infinity }, { aa: -1 }, { aa: 1.1 }]) {
    const f = fixture();
    assert.throws(() => withExpressions(f.manager, weights, () => assert.fail('must not render')));
    assert.deepEqual([...f.values.values()], [.3, .2]);
    assert.equal(f.updates(), 0);
  }
  assert.equal(withExpressions(undefined, {}, () => 42), 42);
  assert.throws(() => withExpressions(undefined, { aa: 1 }, () => 42), /モデルにない/);
});
