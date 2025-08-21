export interface UIStatus {
    nav_stack: string[];
    page_level: number;
    loading: boolean;
    quick_load: boolean;
    readonly currentPage: string;
    switchPage: (page: string, page_level: number) => void;
    beginLoading: (loading?: boolean, quick_load?: boolean) => void;
    endLoading: () => void;
}

export function createUIStatusStatic(): UIStatus {
    return <UIStatus>{
        nav_stack: [],
        page_level: -1,
        loading: true,
        quick_load: false,
        get currentPage() {
            if (this.nav_stack.length === 0) {
                return null!;
            }
            return this.nav_stack[this.nav_stack.length - 1];
        },
        switchPage(page: string, page_level: number): void {
            if (this.nav_stack.length > 0 && this.nav_stack[this.nav_stack.length - 1] === page) {
                return;
            }
            while (this.page_level >= page_level) {
                this.nav_stack.pop();
            }
            this.nav_stack.push(page);
            this.page_level = page_level;
        },
        beginLoading(loading: boolean = true, quick_load: boolean = false): void {
            this.loading = loading;
            this.quick_load = quick_load;
        },
        endLoading(): void {
            this.loading = false;
        }
    };
}
