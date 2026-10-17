import { test } from 'node:test';
import assert from 'node:assert/strict';
import { planSceneMove, applySceneMove } from './scene-edit.ts';

test('all difference pages and the apply share the original revision and operation', async () => {
  const requests: any[] = [];
  const api = { call: async (name: string, args: any): Promise<unknown> => {
    requests.push({name,args});
    if (name === 'get_project') return {type:'project_read',result:{revision:7}};
    if (args.dry_run) return {type:'edit_preview',preview:{source_revision:7,next_offset:args.offset===0?50:null}};
    return {type:'edited'};
  }};
  const plan = await planSceneMove(api,'session','scene',12000000);
  assert.equal(plan.previews.length,2);
  assert.deepEqual(requests.slice(1).map(r=>r.args.offset),[0,50]);
  assert.ok(requests.slice(1).every(r=>r.args.dry_run && r.args.limit===50));
  await applySceneMove(api,plan);
  assert.ok(requests.slice(1).every(r=>r.args.expected_revision===7));
  assert.ok(requests.slice(1).every(r=>r.args.operation_id===plan.request.operation_id));
  assert.equal(requests.at(-1).args.dry_run,false);
});
test('a failure on a later page never returns an applicable partial plan', async () => {
  let edits=0;
  const api = {call:async(name:string,args:any):Promise<unknown>=>{
    if(name==='get_project') return {type:'project_read',result:{revision:7}};
    assert.equal(args.dry_run,true);edits++;
    if(args.offset===50) throw new Error('CONFLICT');
    return {type:'edit_preview',preview:{source_revision:7,next_offset:50}};
  }};
  await assert.rejects(planSceneMove(api,'session','scene',0),/CONFLICT/);
  assert.equal(edits,2);
});
test('fractional or unsafe times are rejected before connecting to the host', async () => {
  const api={call:async():Promise<unknown>=>{throw new Error('must not call');}};
  for(const value of [-1,0.5,Number.MAX_SAFE_INTEGER+1,NaN]) {
    await assert.rejects(planSceneMove(api,'session','scene',value),/safe integer/);
  }
});
