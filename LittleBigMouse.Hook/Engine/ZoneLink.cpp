#include "ZoneLink.h"

Zone* ZoneLink::At(const double pos) const
{
	auto z = this;
	while(pos > z->To)
	{
		z = z->Next;
	}
	return z->Target;
}

const ZoneLink* ZoneLink::AtPhysical(const double pos) const
{
	auto z = this;
	while(pos >= z->To)
		z = z->Next;

	return z;
}

const ZoneLink* ZoneLink::AtPixel(const long pos) const
{
	auto z = this;
	while(pos >= z->SourceToPixel)
		z = z->Next;

	return z;
}

long ZoneLink::ToTargetPixel(long v) const
{
	const auto sLength = SourceToPixel - SourceFromPixel;
	const auto tLength = TargetToPixel - TargetFromPixel;
	return ((v - SourceFromPixel) * tLength / sLength) + TargetFromPixel;
}

ZoneLink::ZoneLink(
	const double from, 
	const double to, 
	const long sourceFromPixel, 
	const long sourceToPixel, 
	const long targetFromPixel, 
	const long targetToPixel, 
	const double borderResistance,
	const long targetId
)
	: TargetId(targetId),
	From(from), To(to),
	SourceFromPixel(sourceFromPixel), SourceToPixel(sourceToPixel),
	TargetFromPixel(targetFromPixel), TargetToPixel(targetToPixel),
	BorderResistance(borderResistance)
{
	if (BorderResistance <= 0) BorderResistance = 0;
	else BorderResistancePixel = static_cast<long>((borderResistance / (To - From)) * (SourceToPixel - SourceFromPixel));

	Target = nullptr;
	Next = nullptr;
}
