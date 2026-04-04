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
            new GameClass("Aniripsa",   "Aniripsa.png",   new[] { "aniripsa" }),
            new GameClass("Anutrof",    "Anutrof.png",    new[] { "anutrof" }),
            new GameClass("Feca",       "Feca.png",       new[] { "feca" }),
            new GameClass("Forjalanza", "Forjalanza.png", new[] { "forjalanza" }),
            new GameClass("Hipermago",  "Hipermago.png",  new[] { "hipermago" }),
            new GameClass("Ocra",       "Ocra.png",       new[] { "ocra" }),
            new GameClass("Osamodas",   "Osamodas.png",   new[] { "osamodas" }),
            new GameClass("Pandawa",    "Pandawa.png",    new[] { "pandawa" }),
            new GameClass("Sacrógrito", "Sacrogrito.png", new[] { "sacrogrito", "sacrógrito" }),
            new GameClass("Sadida",     "Sadida.png",     new[] { "sadida" }),
            new GameClass("Selotrop",   "Selotrop.png",   new[] { "selotrop" }),
            new GameClass("Sram",       "Sram.png",       new[] { "sram" }),
            new GameClass("Steamer",    "Steamer.png",    new[] { "steamer" }),
            new GameClass("Tymador",    "Tymador.png",    new[] { "tymador" }),
            new GameClass("Uginak",     "Uginak.png",     new[] { "uginak" }),
            new GameClass("Xelor",      "Xelor.png",      new[] { "xelor" }),
            new GameClass("Yopuka",     "Yopuka.png",     new[] { "yopuka" }),
            new GameClass("Zobal",      "Zobal.png",      new[] { "zobal" }),
            new GameClass("Zurcar",     "Zurcar.png",     new[] { "zurcar" }),
        };

        private static readonly Dictionary<string, GameClass> _byAlias;

        static GameClass()
        {
            _byAlias = new Dictionary<string, GameClass>(StringComparer.OrdinalIgnoreCase);
            foreach (var cls in All)
                foreach (var alias in cls.Aliases)
                    _byAlias[alias] = cls;
        }

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
