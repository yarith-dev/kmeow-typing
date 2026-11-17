import { run } from './rust-tools.mjs';
run('cargo', ['test', '--locked', '-p', 'khmer-engine', '--features', 'bundled-dictionaries', ...process.argv.slice(2)]);
