/** dependency-cruiser config (g07): package graph == union of MODULE.DEPENDS_ON, no cycles */
module.exports = {
  forbidden: [
    {
      name: 'no-cycles',
      from: {},
      to: { circular: true },
    },
    {
      name: 'ui-grid-must-not-be-imported-by-playback',
      from: { path: 'packages/playback' },
      to: { path: 'packages/ui-grid' },
    },
    {
      name: 'ui-subscriber-only',
      from: { path: 'packages/(document|editing|playback|bank-source|export-service|persistence)' },
      to: { path: 'packages/ui-grid|packages/app' },
    },
  ],
  options: {
    doNotFollow: { path: 'node_modules' },
    exclude: { path: 'node_modules|target|dist' },
  },
};
