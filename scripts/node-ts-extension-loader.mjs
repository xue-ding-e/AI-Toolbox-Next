export async function resolve(specifier, context, nextResolve) {
  try {
    return await nextResolve(specifier, context);
  } catch (error) {
    if (!shouldTryTypeScriptExtension(specifier, error)) {
      throw error;
    }

    return await nextResolve(`${specifier}.ts`, context);
  }
}

function shouldTryTypeScriptExtension(specifier, error) {
  if (error?.code !== 'ERR_MODULE_NOT_FOUND') {
    return false;
  }

  if (!specifier.startsWith('.') && !specifier.startsWith('/') && !specifier.startsWith('file:')) {
    return false;
  }

  return !hasFileExtension(specifier);
}

function hasFileExtension(specifier) {
  const normalizedSpecifier = specifier.split(/[?#]/, 1)[0];
  const lastSlashIndex = Math.max(
    normalizedSpecifier.lastIndexOf('/'),
    normalizedSpecifier.lastIndexOf('\\'),
  );
  const basename = normalizedSpecifier.slice(lastSlashIndex + 1);
  return basename.includes('.');
}
