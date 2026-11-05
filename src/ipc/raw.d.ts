// Vite `?raw` imports (the browser mocks load the built-in Skill templates).
declare module "*.md?raw" {
  const text: string;
  export default text;
}
