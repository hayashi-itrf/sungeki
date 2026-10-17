import { expect, it } from 'vitest';
import { motionTransform } from './visual-motion';
import type { Transform } from '@sungeki/api-client';
it('interpolates geometry without changing opacity, flip or source', () => {
  const t: Transform = { x:10,y:20,width:100,height:200,opacity:0.6,flip_x:true,
    motion:{end_x:110,end_y:70,end_scale:2,easing:'linear'} };
  expect(motionTransform(t,500,1000)).toEqual({...t,x:60,y:45,width:150,height:300,motion:undefined});
  expect(t.width).toBe(100);
  t.motion!.easing='ease_in_out';
  expect(motionTransform(t,250,1000).x).toBeCloseTo(24.64466094);
  expect(motionTransform(t,1000,1000).width).toBe(200);
});
