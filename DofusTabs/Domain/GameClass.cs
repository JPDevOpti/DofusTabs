using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Text;
using System.Text.RegularExpressions;

namespace DofusTabs.Domain
{
    public sealed class GameClass
    {
        public string CanonicalName { get; }
        public string IconFile { get; }
        public string[] Aliases { get; }

        private GameClass(string canonicalName, string iconFile, string[] aliases)
        {
            CanonicalName = canonicalName;
            IconFile = iconFile;
            Aliases = aliases;
        }

        public static readonly IReadOnlyList<GameClass> All = new[]
        {
            new GameClass("Aniripsa",   "Aniripsa.jpg",   new[] { "aniripsa" }),
            new GameClass("Anutrof",    "Anutrof.jpg",    new[] { "anutrof" }),
            new GameClass("Feca",       "Feca.jpg",       new[] { "feca" }),
            new GameClass("Forjalanza", "Forjalanza.png", new[] { "forjalanza" }),
            new GameClass("Hipermago",  "Hipermago.jpg",  new[] { "hipermago" }),
            new GameClass("Ocra",       "Ocra.jpg",       new[] { "ocra" }),
            new GameClass("Osamodas",   "Osamodas.jpg",   new[] { "osamodas" }),
            new GameClass("Pandawa",    "Pandawa.jpg",    new[] { "pandawa" }),
            new GameClass("Sacrógrito", "Sacrgrito.jpg",  new[] { "sacrogrito", "sacrógrito" }),
            new GameClass("Sadida",     "Sadida.jpg",     new[] { "sadida" }),
            new GameClass("Selotrop",   "Selotrop.jpg",   new[] { "selotrop" }),
            new GameClass("Sram",       "Sram.jpg",       new[] { "sram" }),
            new GameClass("Steamer",    "Steamer.jpg",    new[] { "steamer" }),
            new GameClass("Tymador",    "Tymador.jpg",    new[] { "tymador" }),
            new GameClass("Uginak",     "Uginak.jpg",     new[] { "uginak" }),
            new GameClass("Xelor",      "Xelor.jpg",      new[] { "xelor" }),
            new GameClass("Yopuka",     "Yopuka.jpg",     new[] { "yopuka" }),
            new GameClass("Zobal",      "Zobal.jpg",      new[] { "zobal" }),
            new GameClass("Zurcar",     "Zurcar.jpg",     new[] { "zurcar" }),
        };

        private static readonly Dictionary<string, GameClass> _byAlias;

        static GameClass()
        {
            _byAlias = new Dictionary<string, GameClass>(StringComparer.OrdinalIgnoreCase);
            foreach (var cls in All)
                foreach (var alias in cls.Aliases)
                    _byAlias[alias] = cls;
        }

        /// <summary>Fail-fast: lanza si hay aliases duplicados. Llamar desde App.OnStartup.</summary>
        public static void ValidateNoDuplicateAliases()
        {
            var duplicated = All
                .SelectMany(c => c.Aliases)
                .GroupBy(a => a, StringComparer.OrdinalIgnoreCase)
                .Where(g => g.Count() > 1)
                .Select(g => g.Key)
                .ToList();

            if (duplicated.Count > 0)
                throw new InvalidOperationException(
                    $"Aliases duplicados en GameClass.All: {string.Join(", ", duplicated)}");
        }

        /// <summary>Busca la clase cuyo alias aparece como palabra completa en el texto dado.</summary>
        public static GameClass? ResolveFromText(string text)
        {
            if (string.IsNullOrWhiteSpace(text)) return null;
            string normalized = NormalizeText(text);
            foreach (var kvp in _byAlias)
            {
                if (ContainsWholeWord(normalized, kvp.Key))
                    return kvp.Value;
            }
            return null;
        }

        /// <summary>Normaliza a minúsculas sin diacríticos, reemplazando no-alfanuméricos con espacio.</summary>
        internal static string NormalizeText(string value)
        {
            string nfd = value.Normalize(NormalizationForm.FormD);
            var sb = new StringBuilder(nfd.Length);
            foreach (char c in nfd)
            {
                if (CharUnicodeInfo.GetUnicodeCategory(c) == UnicodeCategory.NonSpacingMark)
                    continue;
                sb.Append(char.IsLetterOrDigit(c) || char.IsWhiteSpace(c)
                    ? char.ToLowerInvariant(c) : ' ');
            }
            return Regex.Replace(sb.ToString(), @"\s+", " ").Trim();
        }

        private static bool ContainsWholeWord(string text, string word)
        {
            return Regex.IsMatch(
                text,
                @"(^|\s)" + Regex.Escape(word) + @"($|\s)",
                RegexOptions.IgnoreCase);
        }
    }
}
