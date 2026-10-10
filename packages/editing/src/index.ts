// <META - FILE SUMMARY - Editing package entry point.>

// `.ts` specifiers, not `.js`: this barrel is the package's runtime entry point and Node's
// type stripping does not remap a `.js` specifier onto a `.ts` file, so a consumer that
// loads it outside a bundler would fail to resolve. Bundlers accept both, Node accepts
// only this one.
export * from './types.ts';
export * from './overlap.ts';
export * from './commands.ts';
export * from './history.ts';