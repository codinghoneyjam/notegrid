// <META - FILE SUMMARY - document->schedule synchronisation policy (T03 SYNC).>

// `.ts` specifiers, not `.js`: this barrel is the package's runtime entry point and
// Node's type stripping does not remap a `.js` specifier onto a `.ts` file.
export * from './types.ts';
export * from './debounce.ts';
export * from './diff.ts';
export * from './mix.ts';
export * from './audition.ts';
export * from './transport.ts';
export * from './sync.ts';