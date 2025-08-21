export interface UIStatus {
    nav_stack: string[];
    isPartialPage: boolean;
    loading: boolean;
    readonly currentPage: string;
}

export function createUIStatusStatic(): UIStatus {
    return <UIStatus>{
        nav_stack: [],
        isPartialPage: false,
        loading: true,
        get currentPage() {
            return this.nav_stack[this.nav_stack.length - 1];
        }
    };
}
