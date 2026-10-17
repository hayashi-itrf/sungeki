import { expect, it } from 'vitest';
import type { Clip } from '@sungeki/api-client';
import { motionCommand } from './motion-command';
it('changes only motion and preserves the starting geometry, flip and fade', () => {
  const clip = { id:'c', transform:{x:10,y:20,width:100,height:200,opacity:.5,flip_x:true,
    fade:{in_us:100,out_us:200}} } as Clip;
  const motion = {end_x:20,end_y:30,end_scale:1.2,easing:'linear' as const};
  const command=motionCommand(clip,motion);
  expect(command).toEqual({type:'set_transform',clip_id:'c',transform:{...clip.transform,motion}});
  expect(clip.transform.motion).toBeUndefined();
  const enabled = {...clip, transform:{...clip.transform,motion}};
  const clear=motionCommand(enabled,undefined);
  expect(JSON.parse(JSON.stringify(clear))).toEqual({type:'set_transform',clip_id:'c',transform:clip.transform});
});
