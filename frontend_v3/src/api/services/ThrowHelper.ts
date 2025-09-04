export default function ThrowHelper(message: string):never {
    console.error(message);
    throw new Error(message); // Debug时启用此行可以抛出错误中断以便调试，但可能引起副作用
}
