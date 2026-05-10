import { register } from 'node:module';
import { pathToFileURL } from 'node:url';

register(new URL('./node-ts-extension-loader.mjs', import.meta.url), pathToFileURL('./'));
