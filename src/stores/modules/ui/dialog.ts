import { defineStore } from 'pinia'

interface DialogState {
  isOpen: boolean
  type: 'alert' | 'confirm'
  title: string
  message: string
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  resolvePromise: ((value?: any) => void) | null
}

export const useDialogStore = defineStore('dialog', {
  state: (): DialogState => ({
    isOpen: false,
    type: 'alert',
    title: '',
    message: '',
    resolvePromise: null,
  }),

  actions: {
    alert(message: string, title?: string): Promise<void> {
      if (this.isOpen && this.resolvePromise) {
        this.resolvePromise(undefined)
      }
      return new Promise<void>((resolve) => {
        this.isOpen = true
        this.type = 'alert'
        this.title = title || '提示'
        this.message = message
        this.resolvePromise = resolve
      })
    },

    confirm(message: string, title?: string): Promise<boolean> {
      if (this.isOpen && this.resolvePromise) {
        this.resolvePromise(false)
      }
      return new Promise<boolean>((resolve) => {
        this.isOpen = true
        this.type = 'confirm'
        this.title = title || '确认'
        this.message = message
        this.resolvePromise = resolve
      })
    },

    ok() {
      if (this.resolvePromise) {
        this.resolvePromise(this.type === 'confirm' ? true : undefined)
      }
      this.closeDialog()
    },

    cancel() {
      if (this.resolvePromise) {
        this.resolvePromise(false)
      }
      this.closeDialog()
    },

    closeDialog() {
      this.isOpen = false
      this.resolvePromise = null
    },
  },
})
