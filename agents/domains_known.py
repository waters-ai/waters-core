"""domains_knowm.py — стартовая база рейтингов доменов."""

KNOWN_DOMAINS = {
    "wikipedia.org": 0.85,
    "habr.com": 0.80,
    "vc.ru": 0.75,
    "github.com": 0.80,
    "gitlab.com": 0.75,
    "arxiv.org": 0.92,
    "nature.com": 0.92,
    "science.org": 0.90,
    "sciencedirect.com": 0.90,
    "ieee.org": 0.88,
    "acm.org": 0.88,
    "forbes.com": 0.75,
    "techcrunch.com": 0.75,
    "theverge.com": 0.73,
    "medium.com": 0.65,
    "stackoverflow.com": 0.80,
    "nginx.org": 0.85,
    "docker.com": 0.85,
    "python.org": 0.85,
    "react.dev": 0.85,
    "kubernetes.io": 0.85,
    "edu": 0.82,
    "gov": 0.85,
    "mil": 0.85,
    "esa.int": 0.90,
    "nasa.gov": 0.90,
    "who.int": 0.85,
    "europa.eu": 0.82,
    "nih.gov": 0.88,
    "scholar.google.com": 0.90,
    "dtf.ru": 0.70,
    "tproger.ru": 0.72,
    "habr.com": 0.80,
    "regnum.ru": 0.60,
    "ria.ru": 0.65,
    "tass.ru": 0.65,
    "kommersant.ru": 0.70,
    "rbc.ru": 0.68,
    "cnews.ru": 0.62,
    "3dnews.ru": 0.60,
    "habr.com": 0.80,
    "news.ycombinator.com": 0.80,
    "reddit.com": 0.55,
    "stackexchange.com": 0.75,
    "quora.com": 0.45,
}

DEFAULT_RATING_NEW = 0.50
DEFAULT_RATING_INTERNET_KNOWN = 0.55
TRUSTED_THRESHOLD = 0.80
MEDIUM_THRESHOLD = 0.50
TRASH_THRESHOLD = 0.30


def domain_rating(domain: str) -> float:
    if not domain:
        return DEFAULT_RATING_NEW
    domain = domain.lower().strip()
    if domain.startswith("www."):
        domain = domain[4:]
    for known_domain, rating in KNOWN_DOMAINS.items():
        if known_domain.startswith("."):
            if domain.endswith(known_domain):
                return rating
        elif known_domain in domain or domain == known_domain:
            return rating
    return DEFAULT_RATING_NEW
