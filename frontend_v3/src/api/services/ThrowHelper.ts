export default function ThrowHelper(message: string): never {
    console.error(message);
    // throw new Error(message);
    throw message;
}
