const multipleIconsRegex = /(.*)(\[.+?\]$)/;

export type IconDeciderFunction<T = any> = (
  base: string,
  options: string[],
  extraArgs: T,
) => string;

export const iconAssetUrlName = <T = any>(
  assetPath: string,
  deciderFunction: IconDeciderFunction<T> | undefined = undefined,
  extraArgs: T | undefined = undefined,
): string => {
  const {
    public: { iconDomain },
  } = useRuntimeConfig();

  if (!iconDomain) {
    return "";
  }

  const matches = assetPath.match(multipleIconsRegex);
  if (!matches) {
    return `${iconDomain}${
      iconDomain.endsWith("/") ? "" : "/"
    }${assetPath}.png`;
  }

  const base = matches[1];
  const options = matches[2].slice(1, -1).split(",");

  return internalIconDeciderFunction(
    `${iconDomain}${iconDomain.endsWith("/") ? "" : "/"}${base}`,
    options,
    deciderFunction,
    extraArgs,
  );
};

const internalIconDeciderFunction = <T = any | undefined>(
  base: string,
  options: string[],
  deciderFunction: IconDeciderFunction<T> | undefined = undefined,
  extraArgs: T | undefined = undefined,
): string => {
  const deciderFunctionToUse =
    deciderFunction ||
    ((base: string, options: string[]) => `${base}${options[0]}`);
  return deciderFunctionToUse(base, options, extraArgs) + ".png";
};

export const iconAssetUrlNameAmount = (
  assetPath: string,
  extraArgs: number | undefined = undefined,
): string => {
  const {
    public: { iconDomain },
  } = useRuntimeConfig();

  if (!iconDomain) {
    return "";
  }

  const localExtraArgs = extraArgs ? 1 : parseInt(extraArgs);
  const deciderFunctionToUse = (
    base: string,
    options: string[],
    amount: number,
  ) => {
    for (let i = 0; i < options.length; i++) {
      let current = options[i] === "" ? 1 : parseInt(options[i]);
      let next = parseInt(options[i + 1]);

      if (current === amount) {
        return `${base}${options[i]}`;
      }

      if (amount > current && amount < next) {
        return `${base}${options[i]}`;
      }

      if (i === options.length - 1 && amount !== undefined) {
        return `${base}${options[i]}`;
      }
    }

    return `${base}${options[0]}`;
  };

  const matches = assetPath.match(multipleIconsRegex);
  if (!matches) {
    return `${iconDomain}${
      iconDomain.endsWith("/") ? "" : "/"
    }${assetPath}.png`;
  }

  const base = matches[1];
  const options = matches[2].slice(1, -1).split(",");

  return internalIconDeciderFunction<number>(
    `${iconDomain}${iconDomain.endsWith("/") ? "" : "/"}${base}`,
    options,
    deciderFunctionToUse,
    localExtraArgs,
  );
};

export const iconAssetUrlNameRandom = (assetPath: string): string => {
  const {
    public: { iconDomain },
  } = useRuntimeConfig();

  if (!iconDomain) {
    return "";
  }

  const deciderFunctionToUse = (base: string, options: string[]) => {
    const randomIndex = Math.floor(Math.random() * options.length);
    return `${base}${options[randomIndex]}`;
  };

  const matches = assetPath.match(multipleIconsRegex);
  if (!matches) {
    return `${iconDomain}${
      iconDomain.endsWith("/") ? "" : "/"
    }${assetPath}.png`;
  }

  const base = matches[1];
  const options = matches[2].slice(1, -1).split(",");

  return internalIconDeciderFunction<number>(
    `${iconDomain}${iconDomain.endsWith("/") ? "" : "/"}${base}`,
    options,
    deciderFunctionToUse,
  );
};

export const iconAssetUrlNameAll = (assetPath: string): string[] => {
  const {
    public: { iconDomain },
  } = useRuntimeConfig();

  if (!iconDomain) {
    return [];
  }

  const matches = assetPath.match(multipleIconsRegex);

  if (!matches) {
    return [
      `${iconDomain}${iconDomain.endsWith("/") ? "" : "/"}${assetPath}.png`,
    ];
  }

  let icons: string[] = [];

  const base = matches[1];
  const options = matches[2].slice(1, -1).split(",");

  for (let i = 0; i < options.length; i++) {
    icons.push(
      `${iconDomain}${iconDomain.endsWith("/") ? "" : "/"}${base}${options[
        i
      ].trim()}`,
    );
  }

  return icons;
};
