export interface UIStatus {
    __nav_stack: string[];
    isLoading: boolean;
    isFastLoad: boolean;
    __load_progress: number;
    readonly currentPage: string;
    readonly loadProgress: number;
    switchPage: (page: string) => UIStatus;
    beginLoading: (fast_load?: boolean) => UIStatus;
    endLoading: () => UIStatus;
    setLoadProgress: (progress: number, relative?: boolean) => UIStatus;
    back: () => UIStatus;
}

export function createUIStatusStatic(beginPage:string): UIStatus {
    return <UIStatus>{
        __nav_stack:[beginPage],
        isLoading: true,
        isFastLoad: false,
        __load_progress: 0,
        get currentPage() {
            if (this.__nav_stack.length === 0) {
                throw new Error("nav_stack is empty.");
            }
            return this.__nav_stack[this.__nav_stack.length - 1];
        },
        get loadProgress() {
            return this.__load_progress;
        },
        switchPage(page: string): UIStatus {
            if (this.__nav_stack.length > 0 && this.currentPage === page) {
                return this;
            }
            const index = this.__nav_stack.indexOf(page);
            if (index !== -1) {
                this.__nav_stack.splice(index+1);
            }
            return this;
        },
        beginLoading(fast_load: boolean = false): UIStatus {
            this.__load_progress = 0;
            this.isLoading = true;
            this.isFastLoad = fast_load;
            return this;
        },
        endLoading(ensure: boolean = true): UIStatus {
            if (ensure) this.__load_progress = 100;
            this.isLoading = false;
            this.isFastLoad = false;
            return this;
        },
        setLoadProgress(progress: number, relative: boolean = false): UIStatus {
            var new_progress = relative ? progress : this.__load_progress + progress;
            this.__load_progress = new_progress < 0 ? 0 : new_progress > 100 ? 100 : new_progress;
            return this;
        },
        back() {
            this.__nav_stack.pop();
            return this;
        }
    };
}
