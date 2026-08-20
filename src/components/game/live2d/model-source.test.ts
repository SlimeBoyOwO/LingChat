import { describe, expect, it } from 'vitest'

import {
  resolveModelReference,
  rewriteModelReferences,
  type Live2dModelSource,
} from './model-source'

describe('resolveModelReference', () => {
  it('resolves nested and Windows-style references', () => {
    expect(resolveModelReference('live2d/import/Nori/Nori.model3.json', './textures/a.png')).toBe(
      'live2d/import/Nori/textures/a.png',
    )
    expect(
      resolveModelReference('live2d\\import\\Nori\\Nori.model3.json', '..\\shared\\a.png'),
    ).toBe('live2d/import/shared/a.png')
  })

  it.each(['https://example.com/a.png', '/tmp/a.png', 'C:\\tmp\\a.png'])(
    'rejects absolute reference %s',
    (reference) => {
      expect(() => resolveModelReference('model/Nori.model3.json', reference)).toThrow(
        'must be relative',
      )
    },
  )

  it('rejects references escaping the role directory', () => {
    expect(() => resolveModelReference('Nori.model3.json', '../outside.png')).toThrow(
      'escapes the role directory',
    )
  })
})

describe('rewriteModelReferences', () => {
  it('rewrites every runtime file reference', async () => {
    const source: Live2dModelSource = {
      Version: 3,
      FileReferences: {
        Moc: 'Nori.moc3',
        Textures: ['textures/a.png'],
        Physics: 'Nori.physics3.json',
        Pose: 'Nori.pose3.json',
        UserData: 'Nori.userdata3.json',
        DisplayInfo: 'Nori.cdi3.json',
        Expressions: [{ Name: 'happy', File: 'expressions/happy.exp3.json' }],
        Motions: {
          Idle: [{ File: 'motions/idle.motion3.json', Sound: 'sounds/idle.wav' }],
        },
      },
    }
    const resolved: string[] = []

    await rewriteModelReferences(source, 'live2d/Nori/Nori.model3.json', async (relative) => {
      resolved.push(relative)
      return `asset://${relative}`
    })

    expect(resolved.sort()).toEqual(
      [
        'live2d/Nori/Nori.moc3',
        'live2d/Nori/Nori.physics3.json',
        'live2d/Nori/Nori.pose3.json',
        'live2d/Nori/Nori.userdata3.json',
        'live2d/Nori/Nori.cdi3.json',
        'live2d/Nori/textures/a.png',
        'live2d/Nori/expressions/happy.exp3.json',
        'live2d/Nori/motions/idle.motion3.json',
        'live2d/Nori/sounds/idle.wav',
      ].sort(),
    )
    expect(source.FileReferences?.Moc).toBe('asset://live2d/Nori/Nori.moc3')
    expect(source.FileReferences?.Motions?.Idle[0].Sound).toBe(
      'asset://live2d/Nori/sounds/idle.wav',
    )
  })

  it('leaves an empty optional motion sound unset', async () => {
    const source: Live2dModelSource = {
      FileReferences: { Motions: { Idle: [{ File: 'idle.motion3.json', Sound: '' }] } },
    }
    const resolved: string[] = []
    await rewriteModelReferences(source, 'Nori.model3.json', async (relative) => {
      resolved.push(relative)
      return `asset://${relative}`
    })
    expect(resolved).toEqual(['idle.motion3.json'])
    expect(source.FileReferences?.Motions?.Idle[0].Sound).toBe('')
  })

  it('rejects a model without FileReferences', async () => {
    await expect(
      rewriteModelReferences({}, 'Nori.model3.json', async (path) => path),
    ).rejects.toThrow('missing FileReferences')
  })
})
