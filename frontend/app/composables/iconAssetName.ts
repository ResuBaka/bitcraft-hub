const multipleIconsRegex = /(.*)(\[.+?\]$)/;

export type IconDeciderFunction<T = any> = (
  base: string,
  options: string[],
  extraArgs: T,
) => IconDeciderFunctionResult;

export type IconAssetUrl = {
  url: string;
  amount?: number;
  show: boolean;
};

export type IconDeciderFunctionResult = {
  url: string;
  amount?: number;
};

export type IconAssetUrlAll = {
  icons: IconDeciderFunctionResult[];
  show: boolean;
};

export const iconAssetUrlName = <T = any>(
  assetPath: string,
  deciderFunction: IconDeciderFunction<T> | undefined = undefined,
  extraArgs: T | undefined = undefined,
): IconAssetUrl => {
  const {
    public: { iconDomain },
  } = useRuntimeConfig();
  assetPath = assetPath.replace("Other/GeneratedIcons", "");

  const matches = assetPath.match(multipleIconsRegex);
  if (!matches || !iconDomain) {
    return {
      url: `${iconDomain}${
        iconDomain.endsWith("/") ? "" : "/"
      }${assetPath}.png`,
      show: !!iconDomain,
    };
  }

  const base = matches[1];
  const options = matches[2].slice(1, -1).split(",");

  const result = {} as IconAssetUrl;

  const { url, amount } = internalIconDeciderFunction(
    `${iconDomain}${iconDomain.endsWith("/") ? "" : "/"}${base}`,
    options,
    deciderFunction,
    extraArgs,
  );

  result.url = url.endsWith(".png") ? url : `${url}.png`;
  if (amount) {
    result.amount = amount;
  }

  return {
    ...result,
    show: !!iconDomain,
  };
};

const internalIconDeciderFunction = <T = any | undefined>(
  base: string,
  options: string[],
  deciderFunction: IconDeciderFunction<T> | undefined = undefined,
  extraArgs: T | undefined = undefined,
): IconDeciderFunctionResult => {
  const deciderFunctionToUse =
    deciderFunction ||
    ((base: string, options: string[]): IconDeciderFunctionResult => ({
      url: `${base}${options[0]}`,
    }));

  const result = {} as IconDeciderFunctionResult;

  const { url, amount } = deciderFunctionToUse(base, options, extraArgs);

  result.url = url.endsWith(".png") ? url : `${url}.png`;
  if (amount) {
    result.amount = amount;
  }

  return result;
};

export const iconAssetUrlNameAmount = (
  assetPath: string,
  extraArgs: number | undefined = undefined,
): IconAssetUrl => {
  const {
    public: { iconDomain },
  } = useRuntimeConfig();
  const matches = assetPath.match(multipleIconsRegex);
  assetPath = assetPath.replace("Other/GeneratedIcons", "");

  if (!matches || !iconDomain) {
    return {
      url: `${iconDomain}${
        iconDomain.endsWith("/") ? "" : "/"
      }${assetPath}.png`,
      show: !!iconDomain,
    };
  }

  const localExtraArgs = extraArgs ? parseInt(extraArgs) : 1;
  const deciderFunctionToUse = (
    base: string,
    options: string[],
    amount: number,
  ): IconDeciderFunctionResult => {
    for (let i = 0; i < options.length; i++) {
      let current = options[i] === "" ? 1 : parseInt(options[i]);
      let next = parseInt(options[i + 1]);

      if (current === amount) {
        return {
          url: `${base}${options[i]}`,
          amount: Math.ceil(amount / current),
        };
      }

      if (amount > current && amount < next) {
        return {
          url: `${base}${options[i]}`,
          amount: Math.ceil(amount / current),
        };
      }

      if (i === options.length - 1 && amount > current && next !== undefined) {
        return {
          url: `${base}${options[i]}`,
          amount: Math.ceil(amount / current),
        };
      }
    }

    return {
      url: `${base}${options[0]}`,
    };
  };

  const base = matches[1];
  const options = matches[2].slice(1, -1).split(",");
  const result = {} as IconDeciderFunctionResult;

  const { url, amount } = internalIconDeciderFunction<number>(
    `${iconDomain}${iconDomain.endsWith("/") ? "" : "/"}${base}`,
    options,
    deciderFunctionToUse,
    localExtraArgs,
  );

  result.url = url.endsWith(".png") ? url : `${url}.png`;
  if (amount) {
    result.amount = amount;
  }

  return {
    ...result,
    show: !!iconDomain,
  };
};

export const iconAssetUrlNameRandom = (assetPath: string): IconAssetUrl => {
  const {
    public: { iconDomain },
  } = useRuntimeConfig();

  assetPath = assetPath.replace("Other/GeneratedIcons", "");

  const matches = assetPath.match(multipleIconsRegex);
  if (!matches || !iconDomain) {
    return {
      url: `${iconDomain}${
        iconDomain.endsWith("/") ? "" : "/"
      }${assetPath}.png`,
      show: !!iconDomain,
    };
  }

  const deciderFunctionToUse = (
    base: string,
    options: string[],
  ): IconDeciderFunctionResult => {
    const randomIndex = Math.floor(Math.random() * options.length);
    return {
      url: `${base}${options[randomIndex]}`,
    };
  };

  const base = matches[1];
  const options = matches[2].slice(1, -1).split(",");
  const result = {} as IconDeciderFunctionResult;

  const { url, amount } = internalIconDeciderFunction<number>(
    `${iconDomain}${iconDomain.endsWith("/") ? "" : "/"}${base}`,
    options,
    deciderFunctionToUse,
  );

  result.url = url;
  if (amount) {
    result.amount = amount;
  }

  return {
    ...result,
    show: !!iconDomain,
  };
};

export const iconAssetUrlNameAll = (assetPath: string): IconAssetUrlAll => {
  const {
    public: { iconDomain },
  } = useRuntimeConfig();
  const matches = assetPath.match(multipleIconsRegex);

  if (!matches || !iconDomain) {
    return {
      icons: [
        {
          url: `${iconDomain}${
            iconDomain.endsWith("/") ? "" : "/"
          }${assetPath}.png`,
        },
      ],
      show: !!iconDomain,
    };
  }

  let icons: IconDeciderFunctionResult[] = [];

  const base = matches[1];
  const options = matches[2].slice(1, -1).split(",");

  for (let i = 0; i < options.length; i++) {
    icons.push({
      url: `${iconDomain}${iconDomain.endsWith("/") ? "" : "/"}${base}${options[
        i
      ].trim()}`,
    });
  }

  return {
    icons,
    show: !!iconDomain,
  };
};
