import { createRequire } from 'node:module';
import { join } from 'node:path';

const requireFromTools = createRequire(
  join(process.env.QUALITYGATE_ESLINT_MODULE_ROOT, 'package.json'),
);
const { parser, plugin } = requireFromTools('typescript-eslint');

export default [{
  files: ['src/**/*.{ts,tsx,js,jsx,mjs,cjs}'],
  languageOptions: {
    parser,
    parserOptions: { projectService: true, tsconfigRootDir: import.meta.dirname },
  },
  plugins: { '@typescript-eslint': plugin },
  rules: { '@typescript-eslint/no-deprecated': 'error' },
}];
