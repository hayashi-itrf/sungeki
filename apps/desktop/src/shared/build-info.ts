// Embedded at build time; independent of the running host and launch time.
declare const __APP_BUILD_INFO__: { version: string; builtAt: string; id: string };
export const buildInfo = __APP_BUILD_INFO__;
