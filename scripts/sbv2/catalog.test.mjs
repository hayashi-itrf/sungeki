import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp, mkdir, writeFile, utimes, realpath, rm, symlink} from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import {offlineCatalog} from './catalog.mjs';
async function fixture(t) {
 const root=await realpath(await mkdtemp(path.join(os.tmpdir(),'sbv2-catalog-')));
 t.after(()=>rm(root,{recursive:true,force:true}));
 async function model(name) {
  const dir=path.join(root,name);await mkdir(dir);
  await writeFile(path.join(dir,'config.json'),JSON.stringify({version:'2.7.0-JP-Extra',data:{sampling_rate:44100,num_styles:2,spk2id:{Voice:4},style2id:{Neutral:0,Happy:1}}}));
  await writeFile(path.join(dir,'style_vectors.npy'),'fixture');
  await writeFile(path.join(dir,'old.pt'),'old');await utimes(path.join(dir,'old.pt'),100,100);
  await writeFile(path.join(dir,'new.safetensors'),'new');await utimes(path.join(dir,'new.safetensors'),200,200);
  return dir;
 }
 return {root,model};
}
test('offline CPU catalog follows directory order and newest weights without loading them',async t=>{
 const {root,model}=await fixture(t);await model('z');await model('a');await model('.hidden');
 const catalog=await offlineCatalog(root);
 assert.equal(Object.keys(catalog).length,2);
 assert.equal(catalog[0].model_path,path.join(root,'a/new.safetensors'));
 assert.equal(catalog[1].config_path,path.join(root,'z/config.json'));
 assert.deepEqual(catalog[0].id2spk,{'4':'Voice'});assert.equal(catalog[0].device,'cpu');
});
test('ambiguous timestamps and linked model directories are rejected',async t=>{
 const {root,model}=await fixture(t),dir=await model('voice');
 await utimes(path.join(dir,'old.pt'),200,200);await assert.rejects(offlineCatalog(root),/同じ更新日時/);
 await utimes(path.join(dir,'old.pt'),100,100);
 await symlink(dir,path.join(root,'alias'));await assert.rejects(offlineCatalog(root),/リンク/);
});
test('missing models and inconsistent config maps cannot produce a catalog',async t=>{
 const {root,model}=await fixture(t);await assert.rejects(offlineCatalog(root),/ありません/);
 const dir=await model('voice');await writeFile(path.join(dir,'config.json'),JSON.stringify({version:'x',data:{sampling_rate:44100,num_styles:1,spk2id:{a:0,b:0},style2id:{Neutral:0}}}));
 await assert.rejects(offlineCatalog(root),/ID/);
});
