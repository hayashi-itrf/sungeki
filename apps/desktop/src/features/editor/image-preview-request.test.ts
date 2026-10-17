import { expect, test, vi } from 'vitest';
import type { SessionRequest, SessionResponse } from '@sungeki/api-client';
import { requestImagePreview } from './image-preview-request';
const request: Extract<SessionRequest, {type:'psd_preview'}> = { type:'psd_preview',session_id:'s',expected_revision:1,clip_id:'c',time_us:0 };
const result:SessionResponse={type:'image_preview',asset_id:'a',sha256:'hash',width:1,height:1,png_base64:'png'};
const busy={code:'CONFLICT',retryable:true};
test('retries only temporary busy and preserves exact request',async()=>{
 const send=vi.fn().mockRejectedValueOnce(busy).mockResolvedValue(result);const wait=vi.fn().mockResolvedValue(undefined);
 expect(await requestImagePreview(send,request,()=>false,wait)).toBe('data:image/png;base64,png');
 expect(send.mock.calls).toEqual([[request],[request]]);expect(wait).toHaveBeenCalledWith(250);
});
test('busy retries stop at a finite limit; stale and missing sources fail immediately',async()=>{
 const wait=vi.fn().mockResolvedValue(undefined);
 const send=vi.fn().mockRejectedValue(busy);
 await expect(requestImagePreview(send,request,()=>false,wait)).rejects.toEqual(busy);expect(send).toHaveBeenCalledTimes(3);
 for(const error of [{code:'CONFLICT',retryable:false},{code:'ASSET_MISSING',retryable:true}]){
  const send=vi.fn().mockRejectedValue(error);await expect(requestImagePreview(send,request,()=>false,wait)).rejects.toEqual(error);expect(send).toHaveBeenCalledTimes(1);
 }
});
test('cancelled views do not retry or display late pixels',async()=>{
 let cancelled=false;
 const send=vi.fn(async()=>{cancelled=true;return result;});
 expect(await requestImagePreview(send,request,()=>cancelled)).toBeNull();
 const uncalled=vi.fn();expect(await requestImagePreview(uncalled,request,()=>true)).toBeNull();expect(uncalled).not.toHaveBeenCalled();
 cancelled=false;const busySend=vi.fn().mockRejectedValue(busy);
 expect(await requestImagePreview(busySend,request,()=>cancelled,async()=>{cancelled=true;})).toBeNull();expect(busySend).toHaveBeenCalledTimes(1);
});
test('video preview retains timeline identity and discards pixels after a seek', async () => {
 const video: Extract<SessionRequest, {type:'video_preview'}> = {type:'video_preview',args:{session_id:'s',expected_revision:7,clip_id:'v',time_us:800000}};
 let moved=false;
 const send=vi.fn(async (received: SessionRequest)=>{expect(received).toEqual(video);moved=true;return result;});
 expect(await requestImagePreview(send,video,()=>moved)).toBeNull();
 moved=false;
 expect(await requestImagePreview(async()=>result,video,()=>moved)).toBe('data:image/png;base64,png');
});
