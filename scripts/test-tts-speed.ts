import assert from 'node:assert/strict'

import { speedToLengthScale } from '../src/utils/tts-speed.ts'

assert.equal(speedToLengthScale(0.5), 0.5)
assert.equal(speedToLengthScale(1), 1)
assert.equal(speedToLengthScale(2), 2)
assert.equal(speedToLengthScale(Number.NaN), 1)
assert.equal(speedToLengthScale(0), 1)

console.log('TTS length-scale mapping tests passed')
