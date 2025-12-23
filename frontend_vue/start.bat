@echo off
call pnpm install
call pnpm format
call pnpm run dev --host
pause