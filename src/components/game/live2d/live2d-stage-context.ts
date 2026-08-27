import type { InjectionKey, Ref } from 'vue'

export interface Live2dStageContext {
  readyRoleIds: Readonly<Ref<ReadonlySet<number>>>
  unavailableRoleIds: Readonly<Ref<ReadonlySet<number>>>
}

export const live2dStageContextKey: InjectionKey<Live2dStageContext> = Symbol('live2d-stage')
