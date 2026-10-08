import { test } from 'node:test';
import assert from 'node:assert/strict';
import type { Project } from '@ng/document';
import { getGMPrograms, getGMFamilies, searchPrograms, ensureProgramLanes } from '../src/palette.ts';
import { getLaneHeaderInfo } from '../src/lane.ts';

function makeProject(): Project {
  return {
    format_version: 1,
    title: 'Test Project',
    ppq: 480,
    tempo: [{ tick: 0, bpm: 120 }],
    meter: [{ tick: 0, num: 4, den: 4 }],
    lanes: [
      { id: 1, name: 'Lane 1', kind: 'melodic', program: 0 },
      { id: 2, name: 'Lane 2', kind: 'melodic', program: 1 },
    ],
    notes: [],
  };
}

test('palette: getGMPrograms returns 128 programs', () => {
  const programs = getGMPrograms();
  assert.equal(programs.length, 128);
  assert.equal(programs[0].program, 0);
  assert.equal(programs[0].name, 'Acoustic Grand Piano');
  assert.equal(programs[0].family, 'Piano');
  assert.equal(programs[127].program, 127);
  assert.equal(programs[127].name, 'Gunshot');
  assert.equal(programs[127].family, 'Sound Effects');
});

test('palette: getGMFamilies returns 16 families', () => {
  const families = getGMFamilies();
  assert.equal(families.length, 16);
  assert.equal(families[0], 'Piano');
  assert.equal(families[15], 'Sound Effects');
});

test('palette: searchPrograms filters by name', () => {
  const results = searchPrograms('Piano');
  assert.ok(results.length > 0);
  assert.ok(results.every(p => p.name.toLowerCase().includes('piano') || p.family.toLowerCase().includes('piano')));
});

test('palette: ensureProgramLanes creates missing lanes', () => {
  const doc = makeProject();
  const programs = [0, 1, 2, 3, 4, 5, 6, 7];
  const newDoc = ensureProgramLanes(doc, programs);
  assert.equal(newDoc.lanes.length, 8);
  assert.equal(newDoc.lanes[2].program, 2);
  assert.equal(newDoc.lanes[7].program, 7);
});

test('palette: ensureProgramLanes does not duplicate existing lanes', () => {
  const doc = makeProject();
  const programs = [0, 1];
  const newDoc = ensureProgramLanes(doc, programs);
  assert.equal(newDoc.lanes.length, 2);
});

test('lane: getLaneHeaderInfo returns correct info', () => {
  const doc = makeProject();
  const info = getLaneHeaderInfo(doc, 1, 24, 0);
  assert.ok(info);
  assert.equal(info!.lane.id, 1);
  assert.equal(info!.y, 0);
  assert.equal(info!.height, 24);
  assert.equal(info!.isMuted, false);
  assert.equal(info!.isSolo, false);
  assert.equal(info!.volume, 127);
  assert.equal(info!.pan, 64);
});

test('lane: getLaneHeaderInfo returns null for missing lane', () => {
  const doc = makeProject();
  const info = getLaneHeaderInfo(doc, 999, 24, 0);
  assert.equal(info, null);
});
