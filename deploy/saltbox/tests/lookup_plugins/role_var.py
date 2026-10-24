"""Stand-in for Saltbox's role_var lookup (lookup_plugins/role_var.py in saltyorg/Saltbox), for rendering
the role off a Saltbox host: `<name><suffix>` (instance scope) wins over `<role>_role<suffix>`."""

from ansible.errors import AnsibleLookupError
from ansible.plugins.lookup import LookupBase

_MISSING = object()


class LookupModule(LookupBase):
    def run(self, terms, variables=None, **kwargs):
        suffix = terms[0]
        role = kwargs.get("role") or variables["role_name"]
        name = self._templar.template(variables.get(f"{role}_name", role))
        for key in (f"{name}{suffix}", f"{role}_role{suffix}"):
            if key in variables:
                return [self._templar.template(variables[key])]
        default = kwargs.get("default", _MISSING)
        if default is _MISSING:
            raise AnsibleLookupError(f"neither {name}{suffix} nor {role}_role{suffix} is defined")
        return [default]
