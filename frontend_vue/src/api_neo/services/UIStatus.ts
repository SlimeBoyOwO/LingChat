export interface UIStatus {
    nav_stack: string[];
    isPartialPage: boolean;
    loading: boolean;
    currentPage: () => string;
}

export async function initUIStatus(): Promise<UIStatus> {
    let result: UIStatus = {
        nav_stack: ["mainMenu"],
        isPartialPage: false,
        loading: true,
        currentPage: undefined!
    };
    result.currentPage = () => {
        return result.nav_stack[result.nav_stack.length - 1];
    };
    return result;
}
