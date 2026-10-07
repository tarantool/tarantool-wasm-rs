import stylistic from '@stylistic/eslint-plugin';
import google from 'eslint-config-google';

// Google publishes a legacy config. Use maintained stylistic implementations
// for rules removed from ESLint; types live in the SDK's TypeScript
// declarations.
const rules = {};
for (const [name, value] of Object.entries(google.rules)) {
  if (['require-jsdoc', 'valid-jsdoc', 'indent', 'max-len'].includes(name)) {
    continue;
  }
  rules[name in stylistic.rules ? `@stylistic/${name}` : name] = value;
}

export default [{
  files: ['src/*.js', 'tests/**/*.js', 'eslint.config.js'],
  ignores: ['tests/components/python-peer/**'],
  plugins: {'@stylistic': stylistic},
  languageOptions: {ecmaVersion: 'latest', sourceType: 'module'},
  rules: {
    ...rules,
    'curly': ['error', 'all'],
    // clang-format may align adjacent array entries into readable columns.
    '@stylistic/no-multi-spaces':
        ['error', {exceptions: {ArrayExpression: true}}],
    '@stylistic/quotes': ['error', 'single', {allowTemplateLiterals: 'always'}],
  },
}];
